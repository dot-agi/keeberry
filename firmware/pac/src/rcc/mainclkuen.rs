#[doc = "Register `MAINCLKUEN` reader"]
pub type R = crate::R<MainclkuenSpec>;
#[doc = "Register `MAINCLKUEN` writer"]
pub type W = crate::W<MainclkuenSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Main clock update enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`mainclkuen::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`mainclkuen::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct MainclkuenSpec;
impl crate::RegisterSpec for MainclkuenSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`mainclkuen::R`](R) reader structure"]
impl crate::Readable for MainclkuenSpec {}
#[doc = "`write(|w| ..)` method takes [`mainclkuen::W`](W) writer structure"]
impl crate::Writable for MainclkuenSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets MAINCLKUEN to value 0"]
impl crate::Resettable for MainclkuenSpec {}
