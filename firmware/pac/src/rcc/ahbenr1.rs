#[doc = "Register `AHBENR1` reader"]
pub type R = crate::R<Ahbenr1Spec>;
#[doc = "Register `AHBENR1` writer"]
pub type W = crate::W<Ahbenr1Spec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "AHB peripheral clock enable register 1\n\nYou can [`read`](crate::Reg::read) this register and get [`ahbenr1::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ahbenr1::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Ahbenr1Spec;
impl crate::RegisterSpec for Ahbenr1Spec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`ahbenr1::R`](R) reader structure"]
impl crate::Readable for Ahbenr1Spec {}
#[doc = "`write(|w| ..)` method takes [`ahbenr1::W`](W) writer structure"]
impl crate::Writable for Ahbenr1Spec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets AHBENR1 to value 0"]
impl crate::Resettable for Ahbenr1Spec {}
