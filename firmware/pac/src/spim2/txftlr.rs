#[doc = "Register `TXFTLR` reader"]
pub type R = crate::R<TxftlrSpec>;
#[doc = "Register `TXFTLR` writer"]
pub type W = crate::W<TxftlrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Transmit FIFO threshold level\n\nYou can [`read`](crate::Reg::read) this register and get [`txftlr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`txftlr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct TxftlrSpec;
impl crate::RegisterSpec for TxftlrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`txftlr::R`](R) reader structure"]
impl crate::Readable for TxftlrSpec {}
#[doc = "`write(|w| ..)` method takes [`txftlr::W`](W) writer structure"]
impl crate::Writable for TxftlrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets TXFTLR to value 0"]
impl crate::Resettable for TxftlrSpec {}
