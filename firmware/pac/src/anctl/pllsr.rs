#[doc = "Register `PLLSR` reader"]
pub type R = crate::R<PllsrSpec>;
#[doc = "Register `PLLSR` writer"]
pub type W = crate::W<PllsrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "PLL status register\n\nYou can [`read`](crate::Reg::read) this register and get [`pllsr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pllsr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct PllsrSpec;
impl crate::RegisterSpec for PllsrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`pllsr::R`](R) reader structure"]
impl crate::Readable for PllsrSpec {}
#[doc = "`write(|w| ..)` method takes [`pllsr::W`](W) writer structure"]
impl crate::Writable for PllsrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets PLLSR to value 0"]
impl crate::Resettable for PllsrSpec {}
