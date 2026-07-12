#[doc = "Register `ESPICR` reader"]
pub type R = crate::R<EspicrSpec>;
#[doc = "Register `ESPICR` writer"]
pub type W = crate::W<EspicrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Enhanced SPI control register\n\nYou can [`read`](crate::Reg::read) this register and get [`espicr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`espicr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct EspicrSpec;
impl crate::RegisterSpec for EspicrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`espicr::R`](R) reader structure"]
impl crate::Readable for EspicrSpec {}
#[doc = "`write(|w| ..)` method takes [`espicr::W`](W) writer structure"]
impl crate::Writable for EspicrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets ESPICR to value 0"]
impl crate::Resettable for EspicrSpec {}
