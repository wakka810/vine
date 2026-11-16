//! Static continuation state, independent of concrete guest values or titles.
//!
//! A signature records the shared pure flag expression at a CFG join.
//! Its operands are SSA edge arguments; equal operands can share phis.
//! Arbitrary entry uses the raw signature. No runtime value guard is added.
use crate::flow_ir::ValueId;
use crate::predicate_ir::Template;
use crate::value_ir::{FlagKind, ScalarWidth};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Binding {
    Constant(u32),
    Extra(u8),
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FlagSignature {
    pub template: usize,
    pub args: [Binding; 4],
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Signature {
    pub flags: [FlagSignature; 6],
    pub extras: u8,
}
#[derive(Clone, Debug)]
pub(crate) struct FlagValue {
    pub template: usize,
    pub args: [ValueId; 4],
    pub payload: ValueId,
}
pub(crate) struct Templates {
    pub values: Vec<Template>,
    interned: BTreeMap<Template, usize>,
    leaves: BTreeMap<(u8, FlagKind, ScalarWidth), usize>,
    results: BTreeMap<(FlagKind, ScalarWidth), usize>,
    pub identity: usize,
}
impl Templates {
    pub fn new() -> Self {
        let mut result = Self {
            values: vec![],
            interned: BTreeMap::new(),
            leaves: BTreeMap::new(),
            results: BTreeMap::new(),
            identity: 0,
        };
        result.identity = result.intern(Template::identity());
        result
    }
    fn intern(&mut self, template: Template) -> usize {
        if let Some(id) = self.interned.get(&template) {
            return *id;
        }
        let id = self.values.len();
        self.interned.insert(template.clone(), id);
        self.values.push(template);
        id
    }
    pub fn leaf(&mut self, bit: u8, kind: FlagKind, width: ScalarWidth) -> usize {
        let key = (bit, kind, width);
        if let Some(&id) = self.leaves.get(&key) {
            return id;
        }
        let id = self.intern(Template::flag(bit, kind, width));
        self.leaves.insert(key, id);
        id
    }
    pub fn result(&mut self, kind: FlagKind, width: ScalarWidth) -> usize {
        let key = (kind, width);
        if let Some(&id) = self.results.get(&key) {
            return id;
        }
        let id = self.intern(Template::result(kind, width));
        self.results.insert(key, id);
        id
    }
    pub fn raw(&self) -> Signature {
        Signature {
            flags: std::array::from_fn(|bit| FlagSignature {
                template: self.identity,
                args: [
                    Binding::Extra(bit as u8),
                    Binding::Constant(0),
                    Binding::Constant(0),
                    Binding::Constant(0),
                ],
            }),
            extras: 6,
        }
    }
    pub fn join_signature(&self, recipes: [usize; 6]) -> Signature {
        let mut extras = 0;
        let flags = recipes.map(|template| FlagSignature {
            template,
            args: std::array::from_fn(|arg| {
                if self.values[template].inputs & (1 << arg) == 0 {
                    Binding::Constant(0)
                } else {
                    let index = extras;
                    extras += 1;
                    Binding::Extra(index)
                }
            }),
        });
        Signature { flags, extras }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn join_layout_preserves_independent_flag_operands_and_raw_entry() {
        let mut templates = Templates::new();
        assert_eq!(
            templates.join_signature([templates.identity; 6]),
            templates.raw()
        );
        let recipes =
            std::array::from_fn(|bit| templates.leaf(bit as u8, FlagKind::Add, ScalarWidth::W32));
        let joined = templates.join_signature(recipes);
        assert_eq!(joined.flags.each_ref().map(|f| f.template), recipes);
        let mut seen = 0;
        for flag in joined.flags {
            for binding in flag.args {
                if let Binding::Extra(index) = binding {
                    assert_eq!(index, seen);
                    seen += 1;
                }
            }
        }
        assert_eq!(seen, joined.extras);
    }
}
