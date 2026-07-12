#[doc = "Register `RX_SAMPLE_DLY` reader"]
pub type R = crate::R<RxSampleDlySpec>;
#[doc = "Register `RX_SAMPLE_DLY` writer"]
pub type W = crate::W<RxSampleDlySpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "RX sample delay register\n\nYou can [`read`](crate::Reg::read) this register and get [`rx_sample_dly::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`rx_sample_dly::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct RxSampleDlySpec;
impl crate::RegisterSpec for RxSampleDlySpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`rx_sample_dly::R`](R) reader structure"]
impl crate::Readable for RxSampleDlySpec {}
#[doc = "`write(|w| ..)` method takes [`rx_sample_dly::W`](W) writer structure"]
impl crate::Writable for RxSampleDlySpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets RX_SAMPLE_DLY to value 0"]
impl crate::Resettable for RxSampleDlySpec {}
