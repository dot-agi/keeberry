#[doc = "Register `OSPEEDR` reader"]
pub type R = crate::R<OspeedrSpec>;
#[doc = "Register `OSPEEDR` writer"]
pub type W = crate::W<OspeedrSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Port output speed register\n\nYou can [`read`](crate::Reg::read) this register and get [`ospeedr::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ospeedr::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct OspeedrSpec;
impl crate::RegisterSpec for OspeedrSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`ospeedr::R`](R) reader structure"]
impl crate::Readable for OspeedrSpec {}
#[doc = "`write(|w| ..)` method takes [`ospeedr::W`](W) writer structure"]
impl crate::Writable for OspeedrSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets OSPEEDR to value 0"]
impl crate::Resettable for OspeedrSpec {}
