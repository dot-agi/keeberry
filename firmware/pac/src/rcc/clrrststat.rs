#[doc = "Register `CLRRSTSTAT` reader"]
pub type R = crate::R<ClrrststatSpec>;
#[doc = "Register `CLRRSTSTAT` writer"]
pub type W = crate::W<ClrrststatSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Clear reset status register\n\nYou can [`read`](crate::Reg::read) this register and get [`clrrststat::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`clrrststat::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct ClrrststatSpec;
impl crate::RegisterSpec for ClrrststatSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`clrrststat::R`](R) reader structure"]
impl crate::Readable for ClrrststatSpec {}
#[doc = "`write(|w| ..)` method takes [`clrrststat::W`](W) writer structure"]
impl crate::Writable for ClrrststatSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets CLRRSTSTAT to value 0"]
impl crate::Resettable for ClrrststatSpec {}
