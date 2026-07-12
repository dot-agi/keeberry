#[doc = "Register `RXUICR` reader"]
pub type R = crate::R<RxuicrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
#[doc = "Receive FIFO underflow interrupt clear register\n\nYou can [`read`](crate::Reg::read) this register and get [`rxuicr::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct RxuicrSpec;
impl crate::RegisterSpec for RxuicrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`rxuicr::R`](R) reader structure"]
impl crate::Readable for RxuicrSpec {}
#[doc = "`reset()` method sets RXUICR to value 0"]
impl crate::Resettable for RxuicrSpec {}
