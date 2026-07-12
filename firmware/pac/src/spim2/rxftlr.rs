#[doc = "Register `RXFTLR` reader"]
pub type R = crate::R<RxftlrSpec>;
#[doc = "Register `RXFTLR` writer"]
pub type W = crate::W<RxftlrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Receive FIFO threshold level\n\nYou can [`read`](crate::Reg::read) this register and get [`rxftlr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`rxftlr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct RxftlrSpec;
impl crate::RegisterSpec for RxftlrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`rxftlr::R`](R) reader structure"]
impl crate::Readable for RxftlrSpec {}
#[doc = "`write(|w| ..)` method takes [`rxftlr::W`](W) writer structure"]
impl crate::Writable for RxftlrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets RXFTLR to value 0"]
impl crate::Resettable for RxftlrSpec {}
