#[doc = "Register `DCSSCR` reader"]
pub type R = crate::R<DcsscrSpec>;
#[doc = "Register `DCSSCR` writer"]
pub type W = crate::W<DcsscrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Clock security system control register\n\nYou can [`read`](crate::Reg::read) this register and get [`dcsscr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dcsscr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct DcsscrSpec;
impl crate::RegisterSpec for DcsscrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`dcsscr::R`](R) reader structure"]
impl crate::Readable for DcsscrSpec {}
#[doc = "`write(|w| ..)` method takes [`dcsscr::W`](W) writer structure"]
impl crate::Writable for DcsscrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets DCSSCR to value 0"]
impl crate::Resettable for DcsscrSpec {}
