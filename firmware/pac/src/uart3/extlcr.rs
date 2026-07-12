#[doc = "Register `EXTLCR` reader"]
pub type R = crate::R<ExtlcrSpec>;
#[doc = "Register `EXTLCR` writer"]
pub type W = crate::W<ExtlcrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Line extended control register\n\nYou can [`read`](crate::Reg::read) this register and get [`extlcr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`extlcr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct ExtlcrSpec;
impl crate::RegisterSpec for ExtlcrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`extlcr::R`](R) reader structure"]
impl crate::Readable for ExtlcrSpec {}
#[doc = "`write(|w| ..)` method takes [`extlcr::W`](W) writer structure"]
impl crate::Writable for ExtlcrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets EXTLCR to value 0"]
impl crate::Resettable for ExtlcrSpec {}
