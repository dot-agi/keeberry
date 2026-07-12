#[doc = "Register `MCLKPRE` reader"]
pub type R = crate::R<MclkpreSpec>;
#[doc = "Register `MCLKPRE` writer"]
pub type W = crate::W<MclkpreSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "MCLK prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`mclkpre::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mclkpre::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct MclkpreSpec;
impl crate::RegisterSpec for MclkpreSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`mclkpre::R`](R) reader structure"]
impl crate::Readable for MclkpreSpec {}
#[doc = "`write(|w| ..)` method takes [`mclkpre::W`](W) writer structure"]
impl crate::Writable for MclkpreSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets MCLKPRE to value 0"]
impl crate::Resettable for MclkpreSpec {}
