#[doc = "Register `RXFLR` reader"]
pub type R = crate::R<RxflrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
#[doc = "Receive FIFO level register\n\nYou can [`read`](crate::Reg::read) this register and get [`rxflr::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct RxflrSpec;
impl crate::RegisterSpec for RxflrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`rxflr::R`](R) reader structure"]
impl crate::Readable for RxflrSpec {}
#[doc = "`reset()` method sets RXFLR to value 0"]
impl crate::Resettable for RxflrSpec {}
