#[doc = "Register `RISR` reader"]
pub type R = crate::R<RisrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
#[doc = "Raw interrupt status register\n\nYou can [`read`](crate::Reg::read) this register and get [`risr::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct RisrSpec;
impl crate::RegisterSpec for RisrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`risr::R`](R) reader structure"]
impl crate::Readable for RisrSpec {}
#[doc = "`reset()` method sets RISR to value 0"]
impl crate::Resettable for RisrSpec {}
