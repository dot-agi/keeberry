#[doc = "Register `TXFLR` reader"]
pub type R = crate::R<TxflrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
#[doc = "Transmit FIFO level register\n\nYou can [`read`](crate::Reg::read) this register and get [`txflr::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct TxflrSpec;
impl crate::RegisterSpec for TxflrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`txflr::R`](R) reader structure"]
impl crate::Readable for TxflrSpec {}
#[doc = "`reset()` method sets TXFLR to value 0"]
impl crate::Resettable for TxflrSpec {}
