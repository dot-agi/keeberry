#[doc = "Register `AHBENR0` reader"]
pub type R = crate::R<Ahbenr0Spec>;
#[doc = "Register `AHBENR0` writer"]
pub type W = crate::W<Ahbenr0Spec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "AHB peripheral clock enable register 0\n\nYou can [`read`](crate::Reg::read) this register and get [`ahbenr0::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ahbenr0::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Ahbenr0Spec;
impl crate::RegisterSpec for Ahbenr0Spec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`ahbenr0::R`](R) reader structure"]
impl crate::Readable for Ahbenr0Spec {}
#[doc = "`write(|w| ..)` method takes [`ahbenr0::W`](W) writer structure"]
impl crate::Writable for Ahbenr0Spec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets AHBENR0 to value 0"]
impl crate::Resettable for Ahbenr0Spec {}
