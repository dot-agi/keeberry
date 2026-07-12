#[doc = "Register `I2SCLKENR` reader"]
pub type R = crate::R<I2sclkenrSpec>;
#[doc = "Register `I2SCLKENR` writer"]
pub type W = crate::W<I2sclkenrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "I2S SCLK enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`i2sclkenr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`i2sclkenr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct I2sclkenrSpec;
impl crate::RegisterSpec for I2sclkenrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`i2sclkenr::R`](R) reader structure"]
impl crate::Readable for I2sclkenrSpec {}
#[doc = "`write(|w| ..)` method takes [`i2sclkenr::W`](W) writer structure"]
impl crate::Writable for I2sclkenrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets I2SCLKENR to value 0"]
impl crate::Resettable for I2sclkenrSpec {}
