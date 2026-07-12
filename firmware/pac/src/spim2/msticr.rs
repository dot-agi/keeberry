#[doc = "Register `MSTICR` reader"]
pub type R = crate::R<MsticrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
#[doc = "Multi-master interrupt clear register\n\nYou can [`read`](crate::Reg::read) this register and get [`msticr::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct MsticrSpec;
impl crate::RegisterSpec for MsticrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`msticr::R`](R) reader structure"]
impl crate::Readable for MsticrSpec {}
#[doc = "`reset()` method sets MSTICR to value 0"]
impl crate::Resettable for MsticrSpec {}
