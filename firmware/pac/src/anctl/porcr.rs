#[doc = "Register `PORCR` reader"]
pub type R = crate::R<PorcrSpec>;
#[doc = "Register `PORCR` writer"]
pub type W = crate::W<PorcrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Power-on reset control register\n\nYou can [`read`](crate::Reg::read) this register and get [`porcr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`porcr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct PorcrSpec;
impl crate::RegisterSpec for PorcrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`porcr::R`](R) reader structure"]
impl crate::Readable for PorcrSpec {}
#[doc = "`write(|w| ..)` method takes [`porcr::W`](W) writer structure"]
impl crate::Writable for PorcrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets PORCR to value 0"]
impl crate::Resettable for PorcrSpec {}
