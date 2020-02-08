use dynasmrt::{AssemblyOffset, DynasmApi};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct OffsetAssembler(usize);

impl OffsetAssembler {
    fn new(start_offset: usize) -> Self {
        Self(start_offset)
    }
}

impl Extend<u8> for OffsetAssembler {
    fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
        self.0 += iter.into_iter().count()
    }
}

impl<'a> Extend<&'a u8> for OffsetAssembler {
    fn extend<T: IntoIterator<Item = &'a u8>>(&mut self, iter: T) {
        self.0 += iter.into_iter().count()
    }
}

impl DynasmApi for OffsetAssembler {
    fn offset(&self) -> AssemblyOffset {
        AssemblyOffset(self.0)
    }

    fn push(&mut self, _byte: u8) {
        self.0 += 1;
    }

    fn align(&mut self, alignment: usize, _with: u8) {
        let remainder = self.0 % alignment;
        if remainder != 0 {
            self.0 += alignment - remainder;
        }
    }

    fn push_i8(&mut self, _value: i8) {
        self.0 += 1;
    }

    fn push_i16(&mut self, _value: i16) {
        self.0 += 2;
    }

    fn push_i32(&mut self, _value: i32) {
        self.0 += 4;
    }

    fn push_i64(&mut self, _value: i64) {
        self.0 += 8;
    }

    fn push_u16(&mut self, _value: u16) {
        self.0 += 2;
    }

    fn push_u32(&mut self, _value: u32) {
        self.0 += 4;
    }

    fn push_u64(&mut self, _value: u64) {
        self.0 += 8;
    }

    fn runtime_error(&self, msg: &'static str) -> ! {
        panic!(msg);
    }
}
