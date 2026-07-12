#[doc = "Register `HSECR0` reader"]
pub type R = crate::R<Hsecr0Spec>;
#[doc = "Register `HSECR0` writer"]
pub type W = crate::W<Hsecr0Spec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "HSE control register 0\n\nYou can [`read`](crate::Reg::read) this register and get [`hsecr0::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`hsecr0::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Hsecr0Spec;
impl crate::RegisterSpec for Hsecr0Spec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`hsecr0::R`](R) reader structure"]
impl crate::Readable for Hsecr0Spec {}
#[doc = "`write(|w| ..)` method takes [`hsecr0::W`](W) writer structure"]
impl crate::Writable for Hsecr0Spec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets HSECR0 to value 0"]
impl crate::Resettable for Hsecr0Spec {}
