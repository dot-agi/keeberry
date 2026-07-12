#[doc = "Register `SFE` reader"]
pub type R = crate::R<SfeSpec>;
#[doc = "Register `SFE` writer"]
pub type W = crate::W<SfeSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Shadow FIFO enable\n\nYou can [`read`](crate::Reg::read) this register and get [`sfe::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`sfe::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct SfeSpec;
impl crate::RegisterSpec for SfeSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`sfe::R`](R) reader structure"]
impl crate::Readable for SfeSpec {}
#[doc = "`write(|w| ..)` method takes [`sfe::W`](W) writer structure"]
impl crate::Writable for SfeSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets SFE to value 0"]
impl crate::Resettable for SfeSpec {}
