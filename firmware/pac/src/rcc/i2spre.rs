#[doc = "Register `I2SPRE` reader"]
pub type R = crate::R<I2spreSpec>;
#[doc = "Register `I2SPRE` writer"]
pub type W = crate::W<I2spreSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "I2S prescaler register\n\nYou can [`read`](crate::Reg::read) this register and get [`i2spre::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`i2spre::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct I2spreSpec;
impl crate::RegisterSpec for I2spreSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`i2spre::R`](R) reader structure"]
impl crate::Readable for I2spreSpec {}
#[doc = "`write(|w| ..)` method takes [`i2spre::W`](W) writer structure"]
impl crate::Writable for I2spreSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets I2SPRE to value 0"]
impl crate::Resettable for I2spreSpec {}
