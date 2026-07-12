#[doc = "Register `FHSIENR` reader"]
pub type R = crate::R<FhsienrSpec>;
#[doc = "Register `FHSIENR` writer"]
pub type W = crate::W<FhsienrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "FHSI oscillator enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`fhsienr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`fhsienr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct FhsienrSpec;
impl crate::RegisterSpec for FhsienrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`fhsienr::R`](R) reader structure"]
impl crate::Readable for FhsienrSpec {}
#[doc = "`write(|w| ..)` method takes [`fhsienr::W`](W) writer structure"]
impl crate::Writable for FhsienrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets FHSIENR to value 0"]
impl crate::Resettable for FhsienrSpec {}
