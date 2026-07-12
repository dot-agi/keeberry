#[doc = "Register `PVDCR` reader"]
pub type R = crate::R<PvdcrSpec>;
#[doc = "Register `PVDCR` writer"]
pub type W = crate::W<PvdcrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Programmable voltage detector control register\n\nYou can [`read`](crate::Reg::read) this register and get [`pvdcr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pvdcr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct PvdcrSpec;
impl crate::RegisterSpec for PvdcrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`pvdcr::R`](R) reader structure"]
impl crate::Readable for PvdcrSpec {}
#[doc = "`write(|w| ..)` method takes [`pvdcr::W`](W) writer structure"]
impl crate::Writable for PvdcrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets PVDCR to value 0"]
impl crate::Resettable for PvdcrSpec {}
