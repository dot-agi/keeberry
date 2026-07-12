#[doc = "Register `RXOICR` reader"]
pub type R = crate::R<RxoicrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
#[doc = "Receive FIFO overflow interrupt clear register\n\nYou can [`read`](crate::Reg::read) this register and get [`rxoicr::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct RxoicrSpec;
impl crate::RegisterSpec for RxoicrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`rxoicr::R`](R) reader structure"]
impl crate::Readable for RxoicrSpec {}
#[doc = "`reset()` method sets RXOICR to value 0"]
impl crate::Resettable for RxoicrSpec {}
