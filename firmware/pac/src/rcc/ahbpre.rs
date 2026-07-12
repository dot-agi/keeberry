#[doc = "Register `AHBPRE` reader"]
pub type R = crate::R<AhbpreSpec>;
#[doc = "Register `AHBPRE` writer"]
pub type W = crate::W<AhbpreSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "AHB prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`ahbpre::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ahbpre::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct AhbpreSpec;
impl crate::RegisterSpec for AhbpreSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`ahbpre::R`](R) reader structure"]
impl crate::Readable for AhbpreSpec {}
#[doc = "`write(|w| ..)` method takes [`ahbpre::W`](W) writer structure"]
impl crate::Writable for AhbpreSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets AHBPRE to value 0"]
impl crate::Resettable for AhbpreSpec {}
