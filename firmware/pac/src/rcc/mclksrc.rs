#[doc = "Register `MCLKSRC` reader"]
pub type R = crate::R<MclksrcSpec>;
#[doc = "Register `MCLKSRC` writer"]
pub type W = crate::W<MclksrcSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "MCLK source register\n\nYou can [`read`](crate::Reg::read) this register and get [`mclksrc::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mclksrc::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct MclksrcSpec;
impl crate::RegisterSpec for MclksrcSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`mclksrc::R`](R) reader structure"]
impl crate::Readable for MclksrcSpec {}
#[doc = "`write(|w| ..)` method takes [`mclksrc::W`](W) writer structure"]
impl crate::Writable for MclksrcSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets MCLKSRC to value 0"]
impl crate::Resettable for MclksrcSpec {}
