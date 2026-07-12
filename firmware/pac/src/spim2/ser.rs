#[doc = "Register `SER` reader"]
pub type R = crate::R<SerSpec>;
#[doc = "Register `SER` writer"]
pub type W = crate::W<SerSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Slave enable register\n\nYou can [`read`](crate::Reg::read) this register and get [`ser::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`ser::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct SerSpec;
impl crate::RegisterSpec for SerSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`ser::R`](R) reader structure"]
impl crate::Readable for SerSpec {}
#[doc = "`write(|w| ..)` method takes [`ser::W`](W) writer structure"]
impl crate::Writable for SerSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets SER to value 0"]
impl crate::Resettable for SerSpec {}
