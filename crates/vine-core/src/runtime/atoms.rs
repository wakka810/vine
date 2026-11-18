use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(crate) enum RuntimeAtomName {
    Integer(u16),
    String(String),
}

impl RuntimeAtomName {
    pub(crate) fn from_string(value: String) -> Result<Self, u32> {
        if value.is_empty() {
            return Err(123);
        }
        if value.encode_utf16().count() > 255 {
            return Err(87);
        }
        if let Some(number) = value.strip_prefix('#')
            && !number.is_empty()
            && number.bytes().all(|byte| byte.is_ascii_digit())
        {
            let atom = number.parse::<u16>().map_err(|_| 87_u32)?;
            return if (1..0xc000).contains(&atom) {
                Ok(Self::Integer(atom))
            } else {
                Err(87)
            };
        }
        Ok(Self::String(value))
    }
}

#[derive(Debug)]
struct AtomEntry {
    name: String,
    references: u16,
}

/// The global table of the guest Windows environment. User/class atoms have
/// their own namespace; host Windows atoms are never exposed to the guest.
#[derive(Debug, Default)]
pub(crate) struct RuntimeGlobalAtoms {
    entries: BTreeMap<u16, AtomEntry>,
    names: BTreeMap<String, u16>,
}

impl RuntimeGlobalAtoms {
    pub(crate) fn find(&self, name: &RuntimeAtomName) -> Option<u16> {
        match name {
            RuntimeAtomName::Integer(atom) => Some(*atom),
            RuntimeAtomName::String(name) => self.names.get(&name.to_uppercase()).copied(),
        }
    }

    pub(crate) fn add(&mut self, name: RuntimeAtomName) -> Result<u16, u32> {
        let name = match name {
            RuntimeAtomName::Integer(atom) => {
                return if (1..0xc000).contains(&atom) {
                    Ok(atom)
                } else {
                    Err(87)
                };
            }
            RuntimeAtomName::String(name) => name,
        };
        let key = name.to_uppercase();
        if let Some(atom) = self.names.get(&key) {
            let entry = self.entries.get_mut(atom).expect("live global atom");
            entry.references = entry.references.checked_add(1).ok_or(8_u32)?;
            return Ok(*atom);
        }
        let atom = (0xc000..=u16::MAX)
            .find(|atom| !self.entries.contains_key(atom))
            .ok_or(8_u32)?;
        self.names.insert(key, atom);
        self.entries.insert(
            atom,
            AtomEntry {
                name,
                references: 1,
            },
        );
        Ok(atom)
    }

    pub(crate) fn delete(&mut self, atom: u16) -> Result<(), u32> {
        if atom == 0 {
            return Err(87);
        }
        if atom < 0xc000 {
            return Ok(());
        }
        let entry = self.entries.get_mut(&atom).ok_or(87_u32)?;
        if entry.references > 1 {
            entry.references -= 1;
        } else {
            self.names.remove(&entry.name.to_uppercase());
            self.entries.remove(&atom);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_exhaustion_preserves_the_live_atom() {
        let mut atoms = RuntimeGlobalAtoms::default();
        let name = RuntimeAtomName::String("Property".to_owned());
        let atom = atoms.add(name.clone()).unwrap();
        atoms.entries.get_mut(&atom).unwrap().references = u16::MAX;
        assert_eq!(atoms.add(name.clone()), Err(8));
        assert_eq!(atoms.find(&name), Some(atom));
        assert_eq!(atoms.entries[&atom].references, u16::MAX);
        atoms.delete(atom).unwrap();
        assert_eq!(atoms.add(name), Ok(atom));
    }
}
