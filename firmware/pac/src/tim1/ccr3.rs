#[doc = "Register `CCR3` reader"]
pub type R = crate::R<Ccr3Spec>;
#[doc = "Register `CCR3` writer"]
pub type W = crate::W<Ccr3Spec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Capture/compare register 3\n\nYou can [`read`](crate::Reg::read) this register and get [`ccr3::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ccr3::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Ccr3Spec;
impl crate::RegisterSpec for Ccr3Spec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`ccr3::R`](R) reader structure"]
impl crate::Readable for Ccr3Spec {}
#[doc = "`write(|w| ..)` method takes [`ccr3::W`](W) writer structure"]
impl crate::Writable for Ccr3Spec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets CCR3 to value 0"]
impl crate::Resettable for Ccr3Spec {}
