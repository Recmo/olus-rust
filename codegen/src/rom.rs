use crate::code;
use dynasm::dynasm;
use dynasmrt::DynasmApi;
use parser::mir::Module;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
pub(crate) struct Layout {
    pub(crate) closures: Vec<usize>,
    pub(crate) imports:  Vec<usize>,
    pub(crate) strings:  Vec<usize>,
}

impl Layout {
    pub(crate) fn dummy(module: &Module) -> Layout {
        const DUMMY_ROM_START: usize = 1 << 20; // 1G
        layout(module, DUMMY_ROM_START)
    }
}

pub(crate) fn layout(module: &Module, rom_start: usize) -> Layout {
    let mut result = Layout::default();
    let mut offset = rom_start;
    for _decl in &module.declarations {
        result.closures.push(offset);
        offset += 8;
    }
    for _import in &module.imports {
        result.imports.push(offset);
        offset += 8;
    }
    for string in &module.strings {
        result.strings.push(offset);
        offset += 4 + string.len();
    }
    result
}

pub(crate) fn compile(
    module: &Module,
    code_layout: &code::Layout,
    rom_start: usize,
) -> (Vec<u8>, Layout) {
    assert_eq!(module.declarations.len(), code_layout.declarations.len());
    assert_eq!(module.imports.len(), code_layout.imports.len());
    let mut rom = dynasmrt::x64::Assembler::new().unwrap();
    for offset in &code_layout.declarations {
        dynasm!(rom
            ; .qword *offset as i64
        );
    }
    for offset in &code_layout.imports {
        dynasm!(rom
            ; .qword *offset as i64
        );
    }
    for string in &module.strings {
        dynasm!(rom
            ; .dword string.len() as i32
            ; .bytes string.bytes()
        );
    }
    let rom = rom.finalize().expect("Finalize after commit.");
    (rom.to_vec(), layout(module, rom_start))
}
