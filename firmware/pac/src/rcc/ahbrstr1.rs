#[doc = "Register `AHBRSTR1` reader"]
pub type R = crate::R<Ahbrstr1Spec>;
#[doc = "Register `AHBRSTR1` writer"]
pub type W = crate::W<Ahbrstr1Spec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "AHB peripheral reset register 1\n\nYou can [`read`](crate::Reg::read) this register and get [`ahbrstr1::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ahbrstr1::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct Ahbrstr1Spec;
impl crate::RegisterSpec for Ahbrstr1Spec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`ahbrstr1::R`](R) reader structure"]
impl crate::Readable for Ahbrstr1Spec {}
#[doc = "`write(|w| ..)` method takes [`ahbrstr1::W`](W) writer structure"]
impl crate::Writable for Ahbrstr1Spec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets AHBRSTR1 to value 0"]
impl crate::Resettable for Ahbrstr1Spec {}
