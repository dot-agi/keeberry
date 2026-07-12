#[doc = "Register `TXOICR` reader"]
pub type R = crate::R<TxoicrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
#[doc = "Transmit FIFO overflow interrupt clear register\n\nYou can [`read`](crate::Reg::read) this register and get [`txoicr::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct TxoicrSpec;
impl crate::RegisterSpec for TxoicrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`txoicr::R`](R) reader structure"]
impl crate::Readable for TxoicrSpec {}
#[doc = "`reset()` method sets TXOICR to value 0"]
impl crate::Resettable for TxoicrSpec {}
