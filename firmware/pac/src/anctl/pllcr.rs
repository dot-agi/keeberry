#[doc = "Register `PLLCR` reader"]
pub type R = crate::R<PllcrSpec>;
#[doc = "Register `PLLCR` writer"]
pub type W = crate::W<PllcrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "PLL control register\n\nYou can [`read`](crate::Reg::read) this register and get [`pllcr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pllcr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct PllcrSpec;
impl crate::RegisterSpec for PllcrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`pllcr::R`](R) reader structure"]
impl crate::Readable for PllcrSpec {}
#[doc = "`write(|w| ..)` method takes [`pllcr::W`](W) writer structure"]
impl crate::Writable for PllcrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets PLLCR to value 0"]
impl crate::Resettable for PllcrSpec {}
