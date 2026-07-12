#[doc = "Register `STET` reader"]
pub type R = crate::R<StetSpec>;
#[doc = "Register `STET` writer"]
pub type W = crate::W<StetSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Shadow TX empty trigger\n\nYou can [`read`](crate::Reg::read) this register and get [`stet::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`stet::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct StetSpec;
impl crate::RegisterSpec for StetSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`stet::R`](R) reader structure"]
impl crate::Readable for StetSpec {}
#[doc = "`write(|w| ..)` method takes [`stet::W`](W) writer structure"]
impl crate::Writable for StetSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets STET to value 0"]
impl crate::Resettable for StetSpec {}
