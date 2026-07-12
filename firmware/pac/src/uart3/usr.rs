#[doc = "Register `USR` reader"]
pub type R = crate::R<UsrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
#[doc = "UART status register\n\nYou can [`read`](crate::Reg::read) this register and get [`usr::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct UsrSpec;
impl crate::RegisterSpec for UsrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`usr::R`](R) reader structure"]
impl crate::Readable for UsrSpec {}
#[doc = "`reset()` method sets USR to value 0"]
impl crate::Resettable for UsrSpec {}
