#[doc = "Register `SARENR` reader"]
pub type R = crate::R<SarenrSpec>;
#[doc = "Register `SARENR` writer"]
pub type W = crate::W<SarenrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "SAR ADC enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`sarenr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`sarenr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct SarenrSpec;
impl crate::RegisterSpec for SarenrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`sarenr::R`](R) reader structure"]
impl crate::Readable for SarenrSpec {}
#[doc = "`write(|w| ..)` method takes [`sarenr::W`](W) writer structure"]
impl crate::Writable for SarenrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets SARENR to value 0"]
impl crate::Resettable for SarenrSpec {}
