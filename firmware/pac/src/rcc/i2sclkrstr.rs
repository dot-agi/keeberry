#[doc = "Register `I2SCLKRSTR` reader"]
pub type R = crate::R<I2sclkrstrSpec>;
#[doc = "Register `I2SCLKRSTR` writer"]
pub type W = crate::W<I2sclkrstrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "I2S SCLK reset register\n\nYou can [`read`](crate::Reg::read) this register and get [`i2sclkrstr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`i2sclkrstr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct I2sclkrstrSpec;
impl crate::RegisterSpec for I2sclkrstrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`i2sclkrstr::R`](R) reader structure"]
impl crate::Readable for I2sclkrstrSpec {}
#[doc = "`write(|w| ..)` method takes [`i2sclkrstr::W`](W) writer structure"]
impl crate::Writable for I2sclkrstrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets I2SCLKRSTR to value 0"]
impl crate::Resettable for I2sclkrstrSpec {}
