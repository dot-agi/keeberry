#[doc = "Register `DLF` reader"]
pub type R = crate::R<DlfSpec>;
#[doc = "Register `DLF` writer"]
pub type W = crate::W<DlfSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Divisor latch fractional value\n\nYou can [`read`](crate::Reg::read) this register and get [`dlf::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dlf::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct DlfSpec;
impl crate::RegisterSpec for DlfSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`dlf::R`](R) reader structure"]
impl crate::Readable for DlfSpec {}
#[doc = "`write(|w| ..)` method takes [`dlf::W`](W) writer structure"]
impl crate::Writable for DlfSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets DLF to value 0"]
impl crate::Resettable for DlfSpec {}
