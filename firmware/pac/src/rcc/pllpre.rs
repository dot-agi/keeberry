#[doc = "Register `PLLPRE` reader"]
pub type R = crate::R<PllpreSpec>;
#[doc = "Register `PLLPRE` writer"]
pub type W = crate::W<PllpreSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "PLL prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`pllpre::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pllpre::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct PllpreSpec;
impl crate::RegisterSpec for PllpreSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`pllpre::R`](R) reader structure"]
impl crate::Readable for PllpreSpec {}
#[doc = "`write(|w| ..)` method takes [`pllpre::W`](W) writer structure"]
impl crate::Writable for PllpreSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets PLLPRE to value 0"]
impl crate::Resettable for PllpreSpec {}
