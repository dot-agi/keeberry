#[doc = "Register `CCMR2` reader"]
pub type R = crate::R<Ccmr2Spec>;
#[doc = "Register `CCMR2` writer"]
pub type W = crate::W<Ccmr2Spec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Capture/compare mode register 2\n\nYou can [`read`](crate::Reg::read) this register and get [`ccmr2::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ccmr2::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Ccmr2Spec;
impl crate::RegisterSpec for Ccmr2Spec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`ccmr2::R`](R) reader structure"]
impl crate::Readable for Ccmr2Spec {}
#[doc = "`write(|w| ..)` method takes [`ccmr2::W`](W) writer structure"]
impl crate::Writable for Ccmr2Spec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets CCMR2 to value 0"]
impl crate::Resettable for Ccmr2Spec {}
