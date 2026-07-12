#[doc = "Register `PLLSRC` reader"]
pub type R = crate::R<PllsrcSpec>;
#[doc = "Register `PLLSRC` writer"]
pub type W = crate::W<PllsrcSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "PLL source register\n\nYou can [`read`](crate::Reg::read) this register and get [`pllsrc::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pllsrc::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct PllsrcSpec;
impl crate::RegisterSpec for PllsrcSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`pllsrc::R`](R) reader structure"]
impl crate::Readable for PllsrcSpec {}
#[doc = "`write(|w| ..)` method takes [`pllsrc::W`](W) writer structure"]
impl crate::Writable for PllsrcSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets PLLSRC to value 0"]
impl crate::Resettable for PllsrcSpec {}
