#[doc = "Register `DLL` reader"]
pub type R = crate::R<DllSpec>;
#[doc = "Register `DLL` writer"]
pub type W = crate::W<DllSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Divisor latch (low)\n\nYou can [`read`](crate::Reg::read) this register and get [`dll::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dll::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct DllSpec;
impl crate::RegisterSpec for DllSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`dll::R`](R) reader structure"]
impl crate::Readable for DllSpec {}
#[doc = "`write(|w| ..)` method takes [`dll::W`](W) writer structure"]
impl crate::Writable for DllSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets DLL to value 0"]
impl crate::Resettable for DllSpec {}
