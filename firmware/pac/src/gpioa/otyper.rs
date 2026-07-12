#[doc = "Register `OTYPER` reader"]
pub type R = crate::R<OtyperSpec>;
#[doc = "Register `OTYPER` writer"]
pub type W = crate::W<OtyperSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Port output type register\n\nYou can [`read`](crate::Reg::read) this register and get [`otyper::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`otyper::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct OtyperSpec;
impl crate::RegisterSpec for OtyperSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`otyper::R`](R) reader structure"]
impl crate::Readable for OtyperSpec {}
#[doc = "`write(|w| ..)` method takes [`otyper::W`](W) writer structure"]
impl crate::Writable for OtyperSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets OTYPER to value 0"]
impl crate::Resettable for OtyperSpec {}
