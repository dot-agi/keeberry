#[doc = "Register `AHBENR2` reader"]
pub type R = crate::R<Ahbenr2Spec>;
#[doc = "Register `AHBENR2` writer"]
pub type W = crate::W<Ahbenr2Spec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "AHB peripheral clock enable register 2\n\nYou can [`read`](crate::Reg::read) this register and get [`ahbenr2::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ahbenr2::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Ahbenr2Spec;
impl crate::RegisterSpec for Ahbenr2Spec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`ahbenr2::R`](R) reader structure"]
impl crate::Readable for Ahbenr2Spec {}
#[doc = "`write(|w| ..)` method takes [`ahbenr2::W`](W) writer structure"]
impl crate::Writable for Ahbenr2Spec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets AHBENR2 to value 0"]
impl crate::Resettable for Ahbenr2Spec {}
