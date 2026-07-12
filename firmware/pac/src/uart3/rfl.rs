#[doc = "Register `RFL` reader"]
pub type R = crate::R<RflSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
#[doc = "Receive FIFO level\n\nYou can [`read`](crate::Reg::read) this register and get [`rfl::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct RflSpec;
impl crate::RegisterSpec for RflSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`rfl::R`](R) reader structure"]
impl crate::Readable for RflSpec {}
#[doc = "`reset()` method sets RFL to value 0"]
impl crate::Resettable for RflSpec {}
