#[doc = "Register `DMATDLR` reader"]
pub type R = crate::R<DmatdlrSpec>;
#[doc = "Register `DMATDLR` writer"]
pub type W = crate::W<DmatdlrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "DMA transmit data level\n\nYou can [`read`](crate::Reg::read) this register and get [`dmatdlr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dmatdlr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct DmatdlrSpec;
impl crate::RegisterSpec for DmatdlrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`dmatdlr::R`](R) reader structure"]
impl crate::Readable for DmatdlrSpec {}
#[doc = "`write(|w| ..)` method takes [`dmatdlr::W`](W) writer structure"]
impl crate::Writable for DmatdlrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets DMATDLR to value 0"]
impl crate::Resettable for DmatdlrSpec {}
