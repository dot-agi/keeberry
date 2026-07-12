#[doc = "Register `CURRENT` reader"]
pub type R = crate::R<CurrentSpec>;
#[doc = "Register `CURRENT` writer"]
pub type W = crate::W<CurrentSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Port drive-current configuration register (vendor extension)\n\nYou can [`read`](crate::Reg::read) this register and get [`current::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`current::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct CurrentSpec;
impl crate::RegisterSpec for CurrentSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`current::R`](R) reader structure"]
impl crate::Readable for CurrentSpec {}
#[doc = "`write(|w| ..)` method takes [`current::W`](W) writer structure"]
impl crate::Writable for CurrentSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets CURRENT to value 0"]
impl crate::Resettable for CurrentSpec {}
