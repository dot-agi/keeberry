#[doc = "Register `RSTSTAT` reader"]
pub type R = crate::R<RststatSpec>;
#[doc = "Register `RSTSTAT` writer"]
pub type W = crate::W<RststatSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Reset status register\n\nYou can [`read`](crate::Reg::read) this register and get [`rststat::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`rststat::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct RststatSpec;
impl crate::RegisterSpec for RststatSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`rststat::R`](R) reader structure"]
impl crate::Readable for RststatSpec {}
#[doc = "`write(|w| ..)` method takes [`rststat::W`](W) writer structure"]
impl crate::Writable for RststatSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets RSTSTAT to value 0"]
impl crate::Resettable for RststatSpec {}
