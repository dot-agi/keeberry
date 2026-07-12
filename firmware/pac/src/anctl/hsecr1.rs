#[doc = "Register `HSECR1` reader"]
pub type R = crate::R<Hsecr1Spec>;
#[doc = "Register `HSECR1` writer"]
pub type W = crate::W<Hsecr1Spec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "HSE control register 1\n\nYou can [`read`](crate::Reg::read) this register and get [`hsecr1::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`hsecr1::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Hsecr1Spec;
impl crate::RegisterSpec for Hsecr1Spec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`hsecr1::R`](R) reader structure"]
impl crate::Readable for Hsecr1Spec {}
#[doc = "`write(|w| ..)` method takes [`hsecr1::W`](W) writer structure"]
impl crate::Writable for Hsecr1Spec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets HSECR1 to value 0"]
impl crate::Resettable for Hsecr1Spec {}
