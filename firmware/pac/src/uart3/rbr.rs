#[doc = "Register `RBR` reader"]
pub type R = crate::R<RbrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
#[doc = "Receive buffer register\n\nYou can [`read`](crate::Reg::read) this register and get [`rbr::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct RbrSpec;
impl crate::RegisterSpec for RbrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`rbr::R`](R) reader structure"]
impl crate::Readable for RbrSpec {}
#[doc = "`reset()` method sets RBR to value 0"]
impl crate::Resettable for RbrSpec {}
