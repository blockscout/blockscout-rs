import React, { ChangeEvent, FormEvent } from 'react';

import { Input, InputGroup, InputLeftAddon, InputRightElement } from '@chakra-ui/input';
import { chakra } from '@chakra-ui/react';
import { useColorModeValue } from '@chakra-ui/color-mode';

import styles from '../styles/search.module.css'

interface Props {
  initialValue: string
}

export const SearchBar = ({initialValue}: Props) => {
  const [ value, setValue ] = React.useState(initialValue);

  const onChange = React.useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setValue(event.target.value);
  }, []);

  const onSubmit = React.useCallback((event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const url = 'search_results?q=' + value;
    window.location.assign(url);
  }, [ value ]);

  return (
    <chakra.form noValidate onSubmit={ onSubmit } display={{ base: 'none', lg: 'block' }} w="100%" className={styles.search}>
    <InputGroup size='md'>
    <InputLeftAddon w="100px" color="#5A349C" fontWeight="bold">Search</InputLeftAddon>
      <Input
        placeholder="by addresses / transactions / block / token... "
        ml="1px"
        onChange={ onChange }
        borderColor={ useColorModeValue('blackAlpha.100', 'whiteAlpha.200') }
        defaultValue={initialValue}
      />

    </InputGroup>
    </chakra.form>

  );
};

