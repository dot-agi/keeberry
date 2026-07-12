#[doc = "Register `MAINCLKSRC` reader"]
pub type R = crate::R<MainclksrcSpec>;
#[doc = "Register `MAINCLKSRC` writer"]
pub type W = crate::W<MainclksrcSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Main clock source register\n\nYou can [`read`](crate::Reg::read) this register and get [`mainclksrc::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mainclksrc::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct MainclksrcSpec;
impl crate::RegisterSpec for MainclksrcSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`mainclksrc::R`](R) reader structure"]
impl crate::Readable for MainclksrcSpec {}
#[doc = "`write(|w| ..)` method takes [`mainclksrc::W`](W) writer structure"]
impl crate::Writable for MainclksrcSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets MAINCLKSRC to value 0"]
impl crate::Resettable for MainclksrcSpec {}
