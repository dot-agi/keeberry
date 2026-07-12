#[doc = "Register `MODER` reader"]
pub type R = crate::R<ModerSpec>;
#[doc = "Register `MODER` writer"]
pub type W = crate::W<ModerSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Port mode register\n\nYou can [`read`](crate::Reg::read) this register and get [`moder::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`moder::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct ModerSpec;
impl crate::RegisterSpec for ModerSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`moder::R`](R) reader structure"]
impl crate::Readable for ModerSpec {}
#[doc = "`write(|w| ..)` method takes [`moder::W`](W) writer structure"]
impl crate::Writable for ModerSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets MODER to value 0"]
impl crate::Resettable for ModerSpec {}
