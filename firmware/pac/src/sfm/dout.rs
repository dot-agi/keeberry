#[doc = "Register `DOUT[%s]` reader"]
pub type R = crate::R<DoutSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
#[doc = "Result register\n\nYou can [`read`](crate::Reg::read) this register and get [`dout::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct DoutSpec;
impl crate::RegisterSpec for DoutSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`dout::R`](R) reader structure"]
impl crate::Readable for DoutSpec {}
#[doc = "`reset()` method sets DOUT[%s] to value 0"]
impl crate::Resettable for DoutSpec {}
