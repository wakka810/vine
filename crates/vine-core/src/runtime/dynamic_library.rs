use super::{Runtime, RuntimeStateError};
use crate::api::{
    ApiCallFrame, ApiFault, ApiHandlerResult, ApiImmediate, GuestCallbackArgument,
    GuestCallbackCompletionPolicy, GuestCallbackRequest,
};
use crate::memory::GuestAddress;
use crate::runtime::{RuntimeGuestImportBinding, RuntimeHandle};
use crate::{ModuleSnapshot, Pe32LoadPlan, RuntimeExportCatalog};

impl Runtime {
    pub(crate) fn free_library(
        &mut self,
        handle: u32,
        frame: &ApiCallFrame,
        sentinel: GuestAddress,
    ) -> Result<ApiHandlerResult, ApiFault> {
        if !self
            .modules
            .library_requires_detach(handle)
            .map_err(|error| ApiFault::from_runtime(error, &frame.endpoint))?
        {
            self.modules
                .release_library_reference(handle)
                .map_err(|error| ApiFault::from_runtime(error, &frame.endpoint))?;
            return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(1))));
        }
        let id = self
            .modules
            .module_id_by_handle(handle)
            .map_err(|error| ApiFault::from_runtime(error, &frame.endpoint))?;
        let image = self
            .modules
            .images()
            .image(id)
            .map_err(|error| ApiFault::new(error.to_string()))?;
        let callback = image.entry_point.map(|target| GuestCallbackRequest {
            target,
            arguments: vec![
                GuestCallbackArgument::U32(image.image_range.start().0),
                GuestCallbackArgument::U32(0),
                GuestCallbackArgument::U32(0),
            ],
            scratch: Vec::new(),
            scratch_pointer_relocations: Vec::new(),
            return_sentinel: sentinel,
            completion_policy: GuestCallbackCompletionPolicy::FreeLibrary { handle },
        });
        if let Some(request) = &callback {
            self.preflight_guest_callback(frame, request)?;
        }
        if let Some(request) = callback {
            self.modules
                .begin_library_release(handle, self.current_thread_id().0)
                .map_err(|error| ApiFault::from_runtime(error, &frame.endpoint))?;
            return Ok(ApiHandlerResult::GuestCallback(Box::new(request)));
        }
        let mut modules = self.modules.clone();
        modules
            .begin_library_release(handle, self.current_thread_id().0)
            .map_err(|error| ApiFault::from_runtime(error, &frame.endpoint))?;
        modules
            .finish_library_release(handle, self.current_thread_id().0)
            .map_err(|error| ApiFault::from_runtime(error, &frame.endpoint))?;
        self.release_library_memory(handle, modules)
            .map_err(|error| ApiFault::from_runtime(error, &frame.endpoint))?;
        Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(1))))
    }

    pub(crate) fn finish_library_release(&mut self, handle: u32) -> Result<(), RuntimeStateError> {
        let mut modules = self.modules.clone();
        modules.finish_library_release(handle, self.current_thread_id().0)?;
        self.release_library_memory(handle, modules)
    }

    fn release_library_memory(
        &mut self,
        handle: u32,
        modules: crate::runtime::modules::RuntimeModules,
    ) -> Result<(), RuntimeStateError> {
        let id = self.modules.module_id_by_handle(handle)?;
        let image = self
            .modules
            .images()
            .image(id)
            .map_err(|error| RuntimeStateError::new(error.to_string()))?
            .clone();
        let mut objects = self.objects.clone();
        let mut tls = self.image_tls.clone();
        let mut allocator = self.address_space_allocator.clone();
        objects.retire_module(RuntimeHandle(handle))?;
        let mut memory = self.memory.begin_transaction();
        tls.remove_module(&mut memory, &mut allocator, handle)?;
        memory
            .release_region(image.memory_owner, image.image_range.start())
            .map_err(RuntimeStateError::from_memory)?;
        memory
            .destroy_owner(image.memory_owner)
            .map_err(RuntimeStateError::from_memory)?;
        memory.commit_transaction();
        self.modules = modules;
        self.objects = objects;
        self.image_tls = tls;
        self.address_space_allocator = allocator;
        Ok(())
    }

    /// Materialize and bind one reached library. Existing dependencies retain their
    /// identity; missing guest dependencies are rejected before publication.
    #[allow(clippy::too_many_arguments)]
    pub fn load_dynamic_library(
        &mut self,
        snapshot: &ModuleSnapshot,
        loader: &Pe32LoadPlan,
        requested_name: &str,
        load_base: GuestAddress,
        catalog: &RuntimeExportCatalog,
        system_modules: &[String],
    ) -> Result<RuntimeGuestImportBinding, RuntimeStateError> {
        if loader.image.kind != crate::ImageKind::DynamicLibrary {
            return Err(RuntimeStateError::new(
                "dynamic loading requires a PE32 DLL",
            ));
        }
        self.require_source_file_operation_state()?;
        // Only the small module/object tables are staged. Guest bytes remain in
        // the Runtime's single memory authority and its existing transaction.
        let tebs = if loader.tls.is_some() {
            self.image_tls_tebs()?
        } else {
            Vec::new()
        };
        let mut tls = self.image_tls.clone();
        let mut allocator = self.address_space_allocator.clone();
        let mut modules = self.modules.clone();
        let mut objects = self.objects.clone();
        let handle = modules.stage_dynamic_library(
            &mut objects,
            requested_name,
            catalog,
            system_modules,
            self.current_thread_id().0,
        )?;
        let mut memory = self.memory.begin_transaction();
        let image = crate::materialize::materialize_pe32_image_transaction(
            &mut memory,
            snapshot,
            loader,
            load_base,
        )
        .map_err(|error| RuntimeStateError::new(error.to_string()))?;
        let binding = crate::dll::bind_guest_module_imports_transaction(
            &mut memory,
            &mut modules,
            &self.address_space_allocator,
            handle,
            loader,
            &image,
            catalog,
        )?;
        modules
            .register_image(&memory, handle, loader, &image)
            .map_err(|error| RuntimeStateError::new(error.to_string()))?;
        tls.register_module(&mut memory, &mut allocator, handle, loader, &image, &tebs)?;
        modules.mark_ready(handle)?;
        memory.commit_transaction();
        self.modules = modules;
        self.objects = objects;
        self.image_tls = tls;
        self.address_space_allocator = allocator;
        Ok(binding)
    }

    pub(crate) fn dynamic_library_callback(
        &self,
        handle: u32,
        sentinel: GuestAddress,
    ) -> Result<Option<GuestCallbackRequest>, ApiFault> {
        if !self
            .modules
            .pending_dynamic_library(handle, self.current_thread_id().0)
        {
            return Err(ApiFault::new("LoadLibrary does not own the pending DLL"));
        }
        let id = self
            .modules
            .module_id_by_handle(handle)
            .map_err(|error| ApiFault::new(error.message))?;
        let image = self
            .modules
            .images()
            .image(id)
            .map_err(|error| ApiFault::new(error.to_string()))?;
        Ok(image.entry_point.map(|target| GuestCallbackRequest {
            target,
            arguments: vec![
                GuestCallbackArgument::U32(image.image_range.start().0),
                GuestCallbackArgument::U32(1),
                GuestCallbackArgument::U32(0),
            ],
            scratch: Vec::new(),
            scratch_pointer_relocations: Vec::new(),
            return_sentinel: sentinel,
            completion_policy: GuestCallbackCompletionPolicy::LoadLibrary { handle },
        }))
    }

    pub(crate) fn finish_dynamic_library(
        &mut self,
        handle: u32,
        success: bool,
    ) -> Result<u32, RuntimeStateError> {
        let id = self.modules.module_id_by_handle(handle)?;
        let image = self
            .modules
            .images()
            .image(id)
            .map_err(|error| RuntimeStateError::new(error.to_string()))?
            .clone();
        let mut modules = self.modules.clone();
        modules.finish_dynamic_library(handle, self.current_thread_id().0, success)?;
        if !success {
            let mut objects = self.objects.clone();
            let mut tls = self.image_tls.clone();
            let mut allocator = self.address_space_allocator.clone();
            objects.retire_module(RuntimeHandle(handle))?;
            let mut memory = self.memory.begin_transaction();
            tls.remove_module(&mut memory, &mut allocator, handle)?;
            memory
                .release_region(image.memory_owner, image.image_range.start())
                .map_err(RuntimeStateError::from_memory)?;
            memory
                .destroy_owner(image.memory_owner)
                .map_err(RuntimeStateError::from_memory)?;
            memory.commit_transaction();
            self.objects = objects;
            self.image_tls = tls;
            self.address_space_allocator = allocator;
        }
        self.modules = modules;
        Ok(if success {
            image.image_range.start().0
        } else {
            0
        })
    }
}
